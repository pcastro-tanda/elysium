def change
  change_table :users do |t|
    t.change_default :name, 'unknown'
    t.change_default :address, nil
  end
end
