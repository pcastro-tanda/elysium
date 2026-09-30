def slugs(type, path_prefix)
  expanded_links_item(type)
    .reject { |item| item["base_path"].nil? }
    .map { |item| item["base_path"].gsub(%r{^#{path_prefix}}, "") }
end
