from pathlib import Path

from langchain.agents import AgentType, initialize_agent
from langchain.tools import tool
from langchain_openai import ChatOpenAI
from openskills import OpenSkillRuntime


def load_runtime() -> tuple[OpenSkillRuntime, str]:
    skills_dir = Path(__file__).resolve().parents[3] / "skills"
    runtime = OpenSkillRuntime.from_directory(str(skills_dir))
    runtime.discover_skills()
    catalog = "\n".join(
        f"- {skill['id']}: {skill['description']}"
        for skill in runtime.list_skills()
    )
    return runtime, catalog


runtime, catalog = load_runtime()


@tool("run_skill")
def run_skill(skill_id: str, input: str) -> str:
    """Execute an OpenSkills skill by id with a text input."""
    import json
    result = runtime.execute_skill(
        skill_id,
        input={"query": input},
        timeout_ms=5000,
    )
    output = result.get("output", "")
    # Convert output to string if it's a dict/list
    if isinstance(output, (dict, list)):
        return json.dumps(output, ensure_ascii=False)
    return str(output) if output else ""


llm = ChatOpenAI(model="gpt-4o-mini", temperature=0)
agent = initialize_agent(
    tools=[run_skill],
    llm=llm,
    agent=AgentType.OPENAI_FUNCTIONS,
    verbose=True,
)

response = agent.invoke(
    {
        "input": "\n".join(
            [
                "You can call run_skill to execute OpenSkills skills.",
                "Available skills:",
                catalog,
                "",
                "User request: Summarize the following text using an appropriate skill:",
                "OpenSkills provides a WASM runtime for Claude-compatible skills.",
            ]
        )
    }
)

print(response.get("output", response))
