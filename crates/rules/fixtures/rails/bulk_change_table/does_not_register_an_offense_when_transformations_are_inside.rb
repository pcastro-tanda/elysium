def change
  change_table :users do |t|
    if Rails.env.test?
      t.string :name, null: true
      t.string :address, null: true
    else
      t.string :name, null: false
      t.string :address, null: false
    end
  end
end
