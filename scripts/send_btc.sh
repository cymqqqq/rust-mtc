pid="4uvsa-7fqoo-g5cma-3w24a-me5eu-hl2fe-d7uyc-ubly6-rnm2r-n4tk6-eqe"
dist="tb1q40yh2ck650devsdh6hjwaelh2m5f0xuhgc3arh"
# "tb1puty7fencguj7yezm4u092x9ur0lwd8957m6dz8j87nlj6ctqdznq85fsym"
amount=1000
echo "send btc from $pid to $dist, amount $amount"
dfx canister --network ic call bitcoin_test_backend send_btc \
"record { pid=\"$pid\"; dst_address=\"$dist\"; amount=$amount;}"
