class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
          change_table :users do |t|
        t.change_default :authorized, from: nil, to: 1
      end

  end
end
