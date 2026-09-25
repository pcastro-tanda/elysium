def requested_specs
  @requested_specs ||= begin
    groups = self.groups - Bundler.settings.without
    groups.map! { |g| g.to_sym }
    specs_for(groups)
  end
end
