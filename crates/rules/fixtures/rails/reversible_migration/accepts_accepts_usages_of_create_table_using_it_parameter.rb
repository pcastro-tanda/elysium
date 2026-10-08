class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    create_table :users do
  it.string :name
end

  end
end
