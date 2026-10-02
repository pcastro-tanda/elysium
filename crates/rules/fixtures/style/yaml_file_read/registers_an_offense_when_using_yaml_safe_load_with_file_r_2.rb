YAML.safe_load(File.read(path), permitted_classes: [Regexp, Symbol], aliases: true)
     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `safe_load_file(path, permitted_classes: [Regexp, Symbol], aliases: true)` instead.
