class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    change_table :users do |t|
  t.column :name, :string
  t.text :description
  t.boolean :authorized
end

  end
end
