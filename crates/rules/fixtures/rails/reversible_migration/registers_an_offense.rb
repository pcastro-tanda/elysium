class ::RemoveAnimals < ActiveRecord::Migration[7.0]
  def change
    drop_table :animals
    ^^^^^^^^^^^^^^^^^^^ drop_table(without block) is not reversible.
  end
end
