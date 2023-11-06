use scrypto::prelude::*;

const EPOCH_OF_YEAR: u64 = 105120;
const BABYLON_START_EPOCH: u64 = 32719;
const A_WEEK_EPOCHS: u64 = 60/5*24*7;
const RESERVE_WEEKS: usize = 52;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ScryptoSbor)]
pub struct StakeData{
    pub last_lsu: Decimal,
    pub last_staked: Decimal,
    pub last_stake_epoch: u64
}


#[derive(Debug, Clone, PartialEq, Eq, ScryptoSbor, NonFungibleData)]
pub struct UnstakeData {
    pub name: String,

    /// An epoch number at (or after) which the pending unstaked XRD may be claimed.
    /// Note: on unstake, it is fixed to be [`ConsensusManagerConfigSubstate.num_unstake_epochs`] away.
    pub claim_epoch: Epoch,

    /// An XRD amount to be claimed.
    pub claim_amount: Decimal,
}


#[blueprint]
mod validator_keeper{

    enable_method_auth!{
        roles{
            admin => updatable_by: [];
        },
        methods {
            //admin
            log_validator_staking => restrict_to: [admin, OWNER];

            //public
            get_active_set_apy => PUBLIC;

        }
    }

    struct ValidatorKeeper{
        validator_map: HashMap<ComponentAddress, Vec<StakeData>>,
        last_staked: Decimal,
        last_stake_epoch: u64
    }

    impl ValidatorKeeper {
        pub fn instantiate() -> (Global<ValidatorKeeper>, Bucket){
            let admin_badge = ResourceBuilder::new_fungible(OwnerRole::None)
                //set divisibility to none to ensure that the admin badge can not be fractionalized.
                .divisibility(DIVISIBILITY_NONE)
                .mint_initial_supply(Decimal::ONE);

            let component = Self{
                validator_map: HashMap::new(),
                last_staked: Decimal::ZERO,
                last_stake_epoch: 0u64
            }.instantiate()
            .prepare_to_globalize(
                OwnerRole::Fixed(rule!(require(admin_badge.resource_address())))
            ).roles(
                roles!(
                    admin => rule!(require(admin_badge.resource_address()));
                )
            ).globalize();         
            (component, admin_badge.into())
        }


        pub fn log_validator_staking(&mut self, add_validator_list: Vec<ComponentAddress>, remove_validator_list: Vec<ComponentAddress>) {
            let current_epoch = Runtime::current_epoch().number();
            let current_week_index = Self::get_week_index(current_epoch);
        
            // Remove validators from the map
            remove_validator_list.iter().for_each(|remove_validator_addr| {
                self.validator_map.remove(remove_validator_addr);
            });
        
            // Update staking information for existing validators
            let mut current_staked = self.validator_map.iter_mut()
            .map(|(validator_addr, vec)| {
                let validator: Global<Validator> = Global::from(validator_addr.clone());
                let last_lsu = validator.total_stake_unit_supply();
                let last_staked = validator.total_stake_xrd_amount();
                let latest = vec.first_mut().unwrap();
                let last_index = Self::get_week_index(latest.last_stake_epoch);
                if current_week_index > last_index {
                    vec.insert(0, Self::new_stake_data(last_lsu, last_staked, current_epoch));
                    while vec.capacity() > RESERVE_WEEKS {
                        vec.remove(vec.capacity()-1);
                    }
                }
                else{
                    latest.last_lsu = last_lsu;
                    latest.last_staked = last_staked;
                    latest.last_stake_epoch = current_epoch;
                }
                last_staked
            })
            .fold(Decimal::ZERO, |sum, staked| {
                sum.checked_add(staked).unwrap()
            });

            // Add new validators and update their staking information
            add_validator_list.iter().for_each(|add_validator_addr| {
                if !self.validator_map.contains_key(add_validator_addr) {
                    let staked = self.set_validator_staking(add_validator_addr, current_week_index, current_epoch);
                    current_staked = current_staked.checked_add(staked).unwrap();
                }
            });
        
            self.last_staked = current_staked;
        }
        

        fn set_validator_staking(&mut self, validator_addr: &ComponentAddress, current_week_index: usize, current_epoch: u64) -> Decimal{
            let validator: Global<Validator> = Global::from(validator_addr.clone());
            let last_lsu = validator.total_stake_unit_supply();
            let last_staked = validator.total_stake_xrd_amount();
            self.validator_map.entry(validator_addr.clone()).and_modify(|vec|{
                let latest = vec.first_mut().unwrap();
                let last_index = Self::get_week_index(latest.last_stake_epoch);
                if current_week_index > last_index {
                    vec.insert(0, Self::new_stake_data(last_lsu, last_staked, current_epoch));
                    while vec.capacity() > RESERVE_WEEKS {
                        // queue.pop_back();
                        vec.remove(vec.capacity()-1);
                    }
                }
                else{
                    latest.last_lsu = last_lsu;
                    latest.last_staked = last_staked;
                    latest.last_stake_epoch = current_epoch;
                } 

            }).or_insert(Vec::from([Self::new_stake_data(last_lsu, last_staked, current_epoch)]));
            
            last_staked
        }

        fn new_stake_data(last_lsu: Decimal, last_staked: Decimal, last_stake_epoch: u64) -> StakeData{
            StakeData{
                last_stake_epoch,
                last_lsu,
                last_staked
            }
        }

        fn get_week_index(epoch_at: u64) -> usize{
            // let index: I192 = Decimal::from(epoch_at - BABYLON_START_EPOCH).checked_div(Decimal::from(A_WEEK_EPOCHS)).unwrap()
            // .checked_ceiling().unwrap().try_into();
            // ().to_usize()
            let elapsed_epoch = epoch_at - BABYLON_START_EPOCH;
            let week_index = elapsed_epoch / A_WEEK_EPOCHS;
            let ret =  if week_index * A_WEEK_EPOCHS < elapsed_epoch{
                (week_index + 1) as usize
            }
            else{
                week_index as usize
            };
            ret
        }

        pub fn get_active_set_apy(&self) -> Decimal {
            let current_epoch = Runtime::current_epoch().number();
            let current_week_index = Self::get_week_index(current_epoch);
        
            let (sum, count) = self.validator_map.iter()
                .filter_map(|(validator_addr, vec)| {
                    self.get_validator_apy(validator_addr, vec, current_week_index)
                })
                .fold((Decimal::ZERO, Decimal::ZERO), |(sum, count), apy| {
                    (sum + apy, count + Decimal::ONE)
                });
        
            if count.is_zero() {
                Decimal::ZERO
            } else {
                sum  / count
            }
        }
        

        fn get_validator_apy(&self, _validator_addr: &ComponentAddress, queue: &Vec<StakeData>, current_week_index: usize) -> Option<Decimal> {
            let latest = queue.first()?;
            let latest_week_index = Self::get_week_index(latest.last_stake_epoch);
        
            if latest_week_index != current_week_index {
                return None;
            }
        
            if let Some(previous) = queue.get(1) {
                let previous_week_index = Self::get_week_index(previous.last_stake_epoch);
        
                if previous_week_index == latest_week_index - 1 {
                    let latest_index = latest.last_staked.checked_div(latest.last_lsu)?;
                    let previous_index = previous.last_staked.checked_div(previous.last_lsu)?;
                    let delta_index = latest_index.checked_sub(previous_index)?;
                    let delta_epoch = Decimal::from(latest.last_stake_epoch - previous.last_stake_epoch);
                    return Some((delta_index).checked_mul(Decimal::from(A_WEEK_EPOCHS)).unwrap().checked_div(delta_epoch).unwrap());
                }
            }
        
            None
        }

    }

}