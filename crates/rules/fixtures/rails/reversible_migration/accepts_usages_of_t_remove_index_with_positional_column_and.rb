class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    change_table :users do |t|
  t.remove_index :email, if_exists: true
end

  end
end
