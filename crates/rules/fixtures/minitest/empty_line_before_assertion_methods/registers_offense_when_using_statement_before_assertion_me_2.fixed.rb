def test_do_something
  yaml_load_paths.each do |path|
    YAML.load_file(path)
  rescue Psych::Exception => e
    do_something

    flunk("Error loading #{path}: #{e.inspect}")
  end
end
