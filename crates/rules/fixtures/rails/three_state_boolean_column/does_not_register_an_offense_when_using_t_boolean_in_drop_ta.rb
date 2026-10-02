def change
  drop_table(:users) do |t|
    t.boolean :active
  end
end
