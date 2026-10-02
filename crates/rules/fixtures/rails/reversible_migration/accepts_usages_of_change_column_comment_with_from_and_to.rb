class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
          change_column_comment(:posts, :state, from: nil, to: "draft")

  end
end
