#!/bin/bash
# scripts/get_devnet_funds.sh

WALLET="Ap57C2mt1aNpiS2JhMZFfxDPV4PNyQif4wvTHV3Ap4cq"
KEYPAIR="~/.config/solana/devnet-keypair.json"

echo "🚀 Getting test SOL from Solana Devnet Faucet..."
echo ""

for i in {1..5}; do
  echo "📍 Attempt $i..."

  solana airdrop 10 "$WALLET" \
    --url https://api.devnet.solana.com \
    --keypair "$KEYPAIR" && {
    echo "✅ Success! Got test tokens"
    break
  } || {
    echo "⏳ Retrying in 10s..."
    sleep 10
  }
done

echo ""
echo "✅ Done! Checking balance..."
solana balance --url devnet --keypair "$KEYPAIR"
