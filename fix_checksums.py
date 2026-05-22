import re
import subprocess

def main():
    print("Getting migration info...")
    result = subprocess.run(["cargo", "sqlx", "migrate", "info"], capture_output=True, text=True)
    output = result.stdout
    
    updates = []
    current_version = None
    
    for line in output.split('\n'):
        line = line.strip()
        if not line:
            continue
            
        # Match "20260114233000/installed (different checksum) ..."
        match_version = re.match(r'^(\d+)/installed\s+\(different checksum\)', line)
        if match_version:
            current_version = match_version.group(1)
            continue
            
        # Match "local migration has checksum c010..."
        if current_version and line.startswith("local migration has checksum "):
            checksum = line.split("checksum ")[1].strip()
            sql = f"UPDATE _sqlx_migrations SET checksum = '\\x{checksum}' WHERE version = {current_version};"
            updates.append(sql)
            current_version = None

    if not updates:
        print("No checksum mismatches found.")
        return

    sql_commands = "\n".join(updates)
    print("Executing the following SQL updates:")
    print(sql_commands)
    
    # Run the SQL using psql
    db_url = "postgresql://postgres:password1235@localhost:5433/livana_db"
    psql_process = subprocess.run(["psql", db_url, "-c", sql_commands], capture_output=True, text=True)
    
    if psql_process.returncode == 0:
        print("Successfully updated checksums.")
    else:
        print("Failed to update checksums:")
        print(psql_process.stderr)

if __name__ == "__main__":
    main()
