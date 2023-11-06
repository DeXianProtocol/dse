use scrypto::prelude::*;

#[derive(ScryptoSbor, Eq, PartialEq, Debug, Clone)]
pub enum InterestModel {
    Default,
    StableCoin
}


#[blueprint]
mod interest_model{
    
    struct DefInterestModel{
        validator_keeper: Global<AnyComponent>,
        def_primary: Decimal,
        def_quadratic: Decimal,
        stable_coin_primary: Decimal,
        stable_coin_quadratic: Decimal
    }
    

    impl DefInterestModel {

        pub fn new(keeper_cmp_addr: ComponentAddress, def_primary: Decimal, def_quadratic: Decimal, stable_coin_primary: Decimal, stable_coin_quadratic:Decimal) -> Global<DefInterestModel>{
            Self{
                validator_keeper: Global::from(keeper_cmp_addr),
                def_primary,
                def_quadratic,
                stable_coin_primary,
                stable_coin_quadratic
            }.instantiate().prepare_to_globalize(OwnerRole::None).globalize()
        }

        pub fn get_variable_interest_rate(&self, borrow_ratio: Decimal, model: InterestModel) -> Decimal{
            match model{
                InterestModel::Default => if borrow_ratio > Decimal::ONE {
                    // dec!("0.2") + dec!("0.5")
                    self.def_primary + self.def_quadratic
                }
                else{
                    // 0.2 * r + 0.5 * r**2
                    borrow_ratio.checked_mul(self.def_primary).unwrap()
                    .checked_add(borrow_ratio.checked_powi(2).unwrap().checked_mul(self.def_quadratic).unwrap()).unwrap()
                },
                InterestModel::StableCoin => {
                    let r2 = if borrow_ratio > Decimal::ONE { Decimal::ONE} else{ borrow_ratio.checked_powi(2).unwrap()};
                    let r4 = r2.checked_powi(2).unwrap();
                    let r8 = r2.checked_powi(4).unwrap();
                    // dec!("0.55") * x4  + dec!("0.45")* x8
                    self.stable_coin_primary.checked_mul(r4).unwrap().checked_add(self.stable_coin_quadratic.checked_mul(r8).unwrap()).unwrap()
                }
            }
        }

        pub fn get_stable_interest_rate(&self, borrow_ratio: Decimal, stable_ratio: Decimal, model: InterestModel) -> Decimal{
            let apy = self.get_variable_interest_rate(borrow_ratio, model);
            let validator_apy = self.validator_keeper
                .call_raw::<Decimal>("get_active_set_apy", scrypto_args!());
            if apy > validator_apy {apy} else {validator_apy}
        }
    }


}