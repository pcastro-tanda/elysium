class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    remove_foreign_key :accounts, :branches

  end
end
