class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    change_table :users do |t|
  t.remove_index :email
end

  end
end
