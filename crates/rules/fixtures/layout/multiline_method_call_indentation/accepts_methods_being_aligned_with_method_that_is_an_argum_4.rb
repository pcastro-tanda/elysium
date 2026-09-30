File.read('data.yml')
    .then { YAML.safe_load it }
    .transform_values(&:downcase)
