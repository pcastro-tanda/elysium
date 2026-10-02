class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
          columns = [:foo, :bar]
      change_column_default(*columns, from: nil, to: "draft")

  end
end
