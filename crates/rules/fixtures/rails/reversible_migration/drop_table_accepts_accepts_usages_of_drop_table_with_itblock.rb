class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    drop_table :users do
  it.string :name
end

  end
end
