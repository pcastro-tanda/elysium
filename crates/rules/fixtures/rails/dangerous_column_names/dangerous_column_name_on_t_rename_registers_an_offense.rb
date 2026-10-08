create_table :users do |t|
  t.rename :name, :save
                  ^^^^^ Avoid dangerous column names.
end
