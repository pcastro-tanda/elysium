class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
          change_column_default(:posts, :state, to: "draft", from: nil)

  end
end
