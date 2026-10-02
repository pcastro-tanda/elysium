def change
  reversible do |dir|
    change_table :users do |t|
      dir.up do
        t.string :name, null: false
        t.string :address, null: false
      end

      dir.down do
        t.remove :name
        t.remove :address
      end
    end
  end
end
