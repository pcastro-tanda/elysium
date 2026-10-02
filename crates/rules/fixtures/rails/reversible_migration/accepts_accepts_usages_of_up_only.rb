class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    up_only { execute "UPDATE posts SET published = 'true'" }

  end
end
