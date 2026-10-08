def change
  change_table :users do |t|
    t.column :name, :string, null: false, default: ""
  end
end
