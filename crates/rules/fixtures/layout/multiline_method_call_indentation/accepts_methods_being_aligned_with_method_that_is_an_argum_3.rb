File.read('data.yml')
    .then { YAML.safe_load _1 }
    .transform_values(&:downcase)
