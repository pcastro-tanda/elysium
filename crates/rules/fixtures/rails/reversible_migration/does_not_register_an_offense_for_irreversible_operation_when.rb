class RemoveAnimals < ActiveRecord::Migration[7.0]
  def change
    drop_table :animals
  end
end
