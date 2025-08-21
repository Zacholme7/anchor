#!/bin/bash
for num in 5 13 17 18 19 20 24 25 26 27 30 31 33 36 37 49 53; do 
  name=$(cargo test test_qbft_controller --release 2>&1 | grep "Running test $num$" -A 1 | grep "RUNNING TEST" | cut -d: -f2 | xargs)
  echo -n "Test $num ($name): "
  TEST_FILTER="$name" timeout 5 cargo test test_qbft_controller --lib 2>&1 | grep -q "test.*ok" && echo "PASSES" || echo "FAILS"
done
