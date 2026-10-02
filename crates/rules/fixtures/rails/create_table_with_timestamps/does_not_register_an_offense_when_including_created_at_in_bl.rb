create_table :users do |t|
  t.string :name
  t.string :email

  t.datetime :created_at, default: -> { 'CURRENT_TIMESTAMP' }
end
