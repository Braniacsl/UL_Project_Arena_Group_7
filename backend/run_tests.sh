set -e # Stop on error

echo "1. Nuke everything..."
sudo docker compose down -v

echo "2. Start Database..."
sudo docker compose up -d
echo "Waiting 5 seconds for Postgres to wake up..."
sleep 5

echo "3. Checking Migration File..."
ls -l migrations/
# Ensure we don't have duplicate/conflicting files
if [ ! -f migrations/20241121000000_init.sql ]; then
  echo "Migration file missing! Please ensure migrations/20241121000000_init.sql exists."
  exit 1
fi

echo "4. Running Migrations..."
# Force re-creation of the DB to be safe
cargo sqlx database drop -y || true
cargo sqlx database create
cargo sqlx migrate run

echo "5. VERIFYING DATABASE STATE..."
# We inspect the DB directly to prove the table exists
sudo docker compose exec -T db psql -U postgres -d fyp -c "\dt"

echo "6. Generating SQLx Offline Data..."
# This is the step that was failing. It should work now that we PROVED the table exists.
cargo sqlx prepare -- --lib

echo "7. Running Tests..."
cargo test

echo "SUCCESS! You are ready to push."
$()$(

  ### Run the script
)$()bash
chmod +x fix_db.sh
./fix_db.sh
