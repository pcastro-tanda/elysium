class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    up_only { change_column(:posts, :state, :string) }

  end
end
