class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    remove_foreign_key :accounts, to_table: :branches

  end
end
