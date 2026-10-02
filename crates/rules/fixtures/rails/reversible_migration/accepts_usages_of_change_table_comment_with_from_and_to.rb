class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
          change_table_comment(:posts, from: nil, to: "draft")

  end
end
