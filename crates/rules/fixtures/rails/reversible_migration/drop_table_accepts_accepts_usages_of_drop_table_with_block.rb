class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    drop_table :users do |t|
  t.string :name
end

  end
end
