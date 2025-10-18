#!/bin/bash
echo "Testing TOML parse..."
toml-cli get config/default.toml bot.name 2>&1 || echo "TOML syntax error detected"
chmod +x test_parse.sh
./test_parse.sh
