addr=$(dfx canister --network ic call bitcoin_test_backend get_p2wpkh_address \
"4uvsa-7fqoo-g5cma-3w24a-me5eu-hl2fe-d7uyc-ubly6-rnm2r-n4tk6-eqe")

echo "update utxo for $addr"
dfx canister --network ic call bitcoin_test_backend update_utxo \
"record { address = $addr;}"