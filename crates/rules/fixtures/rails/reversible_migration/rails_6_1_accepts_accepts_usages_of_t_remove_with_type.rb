class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    change_table :users do |t|
  t.remove(:posts, type: :text)
end

  end
end
