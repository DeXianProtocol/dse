use scrypto::prelude::*;
use crate::utils::*;
use crate::keeper::{StakeData, UnstakeData};

#[blueprint]
mod staking_pool {

    // enable_method_auth!{
    //     roles{
    //         pool_owner => updatable_by: [];
    //     },
    //     methods {
    //     }
    // }

    struct StakingResourePool{
        stake_token: ResourceAddress,
        staking_share_res_mgr: ResourceManager,
        validator_map: HashMap<ComponentAddress, StakeData>,
        lsu_map: HashMap<ComponentAddress, Vault>
    }

    impl StakingResourePool {
        
        pub fn instantiate(
            stake_token: ResourceAddress,
            owner_role: OwnerRole,
            pool_mgr_rule: AccessRule,
            address_reservation: Option<GlobalAddressReservation>
        ) -> (Global<StakingResourePool>, ResourceAddress) {
            let (address_reservation, address) =
                Runtime::allocate_component_address(StakingResourePool::blueprint_id());

            let staking_share_res_mgr: ResourceManager = ResourceBuilder::new_fungible(owner_role.clone())
                .metadata(metadata!(init{
                    "pool" => address, locked;
                    "symbol" => "dseXRD", locked;
                    "name" => "DeXian Staking Earning Token ", locked;
                    "icon_url" => "https://dexian.io/images/dse.png", updatable;
                    "info_url" => "https://dexian.io", updatable;

                }))
                .mint_roles(mint_roles! {
                    minter => pool_mgr_rule.clone();
                    minter_updater => rule!(deny_all);
                })
                .burn_roles(burn_roles! {
                    burner => pool_mgr_rule.clone();
                    burner_updater => rule!(deny_all);
                })
                .create_with_no_initial_supply();

            let staking_share_token = staking_share_res_mgr.address();
            let component = Self {
                validator_map: HashMap::new(),
                lsu_map: HashMap::new(),
                stake_token,
                staking_share_res_mgr
            }.instantiate()
            .prepare_to_globalize(owner_role)
            .with_address(address_reservation)
            .globalize();
            
            (component, staking_share_token)
        }

        pub fn contribute(&mut self, bucket: Bucket, validator_addr: ComponentAddress) -> Bucket{
            assert_resource(&bucket.resource_address(), &self.stake_token);
            let (_, _, value_per_share) = self.get_values();
            let join_amount = bucket.amount();
            let share_amount = floor(join_amount.checked_div(value_per_share).unwrap());
            let share_bucket = self.staking_share_res_mgr.mint(share_amount);

            let current_epoch = Runtime::current_epoch().number();
            let mut validator: Global<Validator> = Global::from(validator_addr);
            let lsu = validator.stake(bucket);
            let lsu_amount = lsu.amount();

            self.lsu_map.entry(validator_addr).or_insert(Vault::new(lsu.resource_address())).put(lsu);
            let last_lsu = self.lsu_map.get(&validator_addr).unwrap().amount();
            self.validator_map.entry(validator_addr).and_modify(|stake_data|{
                stake_data.last_staked = validator.get_redemption_value(last_lsu);
                stake_data.last_stake_epoch = current_epoch;
            }).or_insert(
                StakeData { 
                        last_stake_epoch: current_epoch,
                        last_staked: validator.get_redemption_value(lsu_amount),  //join_amount
                        last_lsu
                    }
            );

            share_bucket
        }

        pub fn redeem(&mut self, bucket: Bucket, validator_addr: ComponentAddress) -> Bucket{
            assert_resource(&bucket.resource_address(), &self.staking_share_res_mgr.address());
            assert!(self.lsu_map.contains_key(&validator_addr), "the validator address not exists");
            let (_, _, value_per_share) = self.get_values();
            let redeem_value = bucket.amount().checked_mul(value_per_share).unwrap();
            
            let lsu = self.lsu_map.get_mut(&validator_addr).unwrap();
            let mut validator: Global<Validator> = Global::from(validator_addr);
            let lsu_value = validator.get_redemption_value(lsu.amount());
            
            assert_amount(lsu_value, redeem_value);
            let lsu_index = lsu_value.checked_div(lsu.amount()).unwrap();
            let unstake_lsu_bucket = lsu.take(floor(redeem_value.checked_div(lsu_index).unwrap()));
            let claim_nft = validator.unstake(unstake_lsu_bucket);
            let claim_nft_id = claim_nft.as_non_fungible().non_fungible_local_id();
            let unstake_data = ResourceManager::from_address(claim_nft.resource_address()).get_non_fungible_data::<UnstakeData>(&claim_nft_id);

            self.validator_map.entry(validator_addr).and_modify(|stake_data|{
                stake_data.last_staked = lsu_value.checked_sub(unstake_data.claim_amount).unwrap();
                stake_data.last_stake_epoch = Runtime::current_epoch().number();
            });

            claim_nft
            
        }

        fn get_redemption_value(&self, amount_of_pool_units: Decimal) -> Decimal{
            let(_, _, value_per_share) = self.get_values();
            amount_of_pool_units.checked_mul(value_per_share).unwrap()
        }

        pub fn get_vault_amount(&self) -> Decimal{
            self.sum_current_staked()
        }

        fn get_values(&self) -> (Decimal, Decimal, Decimal){
            let total_value = self.get_vault_amount();
            let staking_share_qty = self.staking_share_res_mgr.total_supply().unwrap();
            (
                total_value,
                staking_share_qty,
                total_value.checked_div(staking_share_qty).unwrap()  //value_per_share
            )
        }

        fn sum_current_staked(& self) -> Decimal {
            let mut sum = Decimal::ZERO;
            for (validator_addr, stake_data) in &self.validator_map{
                let validator: Global<Validator> = Global::from(validator_addr.clone());
                let latest = validator.get_redemption_value(self.lsu_map.get(&validator_addr).unwrap().amount());
                sum = sum.checked_add(latest).unwrap();
            }
            sum
        }
    }
}