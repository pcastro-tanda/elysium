class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    remove_column(:suppliers, :qualification, :string, null: false)

  end
end
