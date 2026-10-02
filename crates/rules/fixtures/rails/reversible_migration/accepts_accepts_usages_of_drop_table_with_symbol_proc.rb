class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    drop_table :users, &:timestamps

  end
end
