const native = require('./openskills_ts.node');

class OpenSkillRuntime {
  constructor(skillsDir) {
    this._inner = new native.OpenSkillRuntimeWrapper(skillsDir);
  }

  loadSkills() {
    return this._inner.load_skills();
  }

  executeSkill(skillId, input, options = {}) {
    const inputJson = JSON.stringify(input ?? {});
    const result = this._inner.execute_skill(
      skillId,
      inputJson,
      options.timeoutMs ?? null
    );
    return {
      output: JSON.parse(result.output_json),
      stdout: result.stdout,
      stderr: result.stderr,
      audit: result.audit,
    };
  }
}

module.exports = {
  OpenSkillRuntime,
};
