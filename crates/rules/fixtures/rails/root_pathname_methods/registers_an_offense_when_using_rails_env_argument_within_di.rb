Dir.glob(Rails.root.join("db", "seeds", Rails.env, "*.rb")).sort.each do |file|
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `Rails.root` is a `Pathname`, so you can use `Rails.root.glob("db/seeds/#{Rails.env}/*.rb")`.
  load file
end
