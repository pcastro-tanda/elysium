def change
  change_table :users do |t|
    t.remove :name, type: :string
  end
end
