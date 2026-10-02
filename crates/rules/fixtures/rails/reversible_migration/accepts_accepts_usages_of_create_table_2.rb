class ExampleMigration < ActiveRecord::Migration[7.0]
  def change
    def change
  change_table :invoices do |t|
    decimals_params = {precision: 10, scale: 2}

    t.decimal :total_discount, decimals_params
  end
end

  end
end
