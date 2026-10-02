sql = User.where('name = :name').select(:id).to_sql

User.where("id IN (#{sql})", name: 'Lastname').first
