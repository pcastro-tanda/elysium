class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    remove_columns(:posts, :title, :body, type: :text)

  end
end
