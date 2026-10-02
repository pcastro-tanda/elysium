def change
  change_table :users do |t|
    t.belongs_to :team
    t.string :name, null: false
  end
end
