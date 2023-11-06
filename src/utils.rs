
use scrypto::prelude::*;


pub fn ceil(dec: Decimal) -> Decimal{
    dec.checked_round(18, RoundingMode::ToPositiveInfinity).unwrap()
}

pub fn floor(dec: Decimal) -> Decimal{
    dec.checked_round(18, RoundingMode::ToNegativeInfinity).unwrap()
}

pub fn assert_resource(res_addr: &ResourceAddress, expect_res_addr: &ResourceAddress){
    assert!(res_addr == expect_res_addr, "the resource address is not expect!");
}

pub fn assert_vault_amount(vault: &Vault, not_less_than: Decimal){
    assert!(!vault.is_empty() && vault.amount() >= not_less_than, "the balance in vault is insufficient.");
}

pub fn assert_amount(v: Decimal, not_less_than: Decimal){
    assert!(v < not_less_than, "target value less than expect!");
}