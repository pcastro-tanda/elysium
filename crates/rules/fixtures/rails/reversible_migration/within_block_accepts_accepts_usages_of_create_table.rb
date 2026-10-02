class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    [:users, :articles].each do |table|
  create_table table do |t|
    t.string :name
  end
end

  end
end
