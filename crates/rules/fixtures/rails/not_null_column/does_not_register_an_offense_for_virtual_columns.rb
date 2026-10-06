add_column :users, :height_in, :virtual, as: "height_cm / 2.54", null: false, default: nil
add_column :users, :height_in, 'virtual', as: "height_cm / 2.54", null: false, default: nil
