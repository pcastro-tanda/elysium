create_table :users do |t|
  t.string :name
  t.string :email

  t.datetime :updated_at, default: -> { 'CURRENT_TIMESTAMP' }
end
