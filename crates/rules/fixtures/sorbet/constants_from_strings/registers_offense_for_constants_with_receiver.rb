klass = Object.constants.select { |c| c.name == "Foo" }
               ^^^^^^^^^ Sorbet/ConstantsFromStrings: Don't use `constants`, it makes the code harder to understand, less editor-friendly, and impossible to analyze. Replace `constants` with a case/when or a hash.
