class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    drop_table :users do
  _1.string :name
end

  end
end
