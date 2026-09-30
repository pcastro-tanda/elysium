while receiver.nil? &&
    !args.empty? &&
    FORBIDDEN_METHODS.include?(method_name)
end
