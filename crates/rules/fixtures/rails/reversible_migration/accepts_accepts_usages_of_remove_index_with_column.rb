class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    remove_index(:posts, column: :body)

  end
end
