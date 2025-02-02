package main

import (
	"context"
	"encoding/json"
	"os"
	"strings"

	"github.com/estuary/flow/go/capture"
	"github.com/estuary/flow/go/flow/ops"
	"github.com/estuary/flow/go/materialize"
	pc "github.com/estuary/flow/go/protocols/capture"
	"github.com/estuary/flow/go/protocols/flow"
	pm "github.com/estuary/flow/go/protocols/materialize"
	"github.com/sirupsen/logrus"
	pb "go.gazette.dev/core/broker/protocol"
	mbp "go.gazette.dev/core/mainboilerplate"
)

type specResponse struct {
	Type               string          `json:"type"`
	DocumentationURL   string          `json:"documentationURL"`
	EndpointSpecSchema json.RawMessage `json:"endpointSpecSchema"`
	ResourceSpecSchema json.RawMessage `json:"resourceSpecSchema"`
}

type apiSpec struct {
	Log         mbp.LogConfig         `group:"Logging" namespace:"log" env-namespace:"LOG"`
	Diagnostics mbp.DiagnosticsConfig `group:"Debug" namespace:"debug" env-namespace:"DEBUG"`
	Image       string                `long:"image" required:"true" description:"Docker image of the connector to use"`
	Network     string                `long:"network" default:"host" description:"The Docker network that connector containers are given access to."`
}

func (cmd apiSpec) execute(ctx context.Context) (specResponse, error) {
	var endpointSpec, err = json.Marshal(struct {
		Image  string   `json:"image"`
		Config struct{} `json:"config"`
	}{Image: cmd.Image})

	if err != nil {
		return specResponse{}, err
	}

	// TODO(johnny): Switch to Docker labels, to annotate Estuary connectors.
	if strings.HasPrefix(cmd.Image, "ghcr.io/estuary/materialize-") {
		return cmd.specMaterialization(ctx, endpointSpec)
	} else {
		return cmd.specCapture(ctx, endpointSpec)
	}
}

func (cmd apiSpec) specCapture(ctx context.Context, spec json.RawMessage) (specResponse, error) {
	var driver, err = capture.NewDriver(ctx,
		flow.EndpointType_AIRBYTE_SOURCE, spec, cmd.Network, ops.StdLogger())
	if err != nil {
		return specResponse{}, err
	}

	resp, err := driver.Spec(ctx, &pc.SpecRequest{
		EndpointType:     flow.EndpointType_AIRBYTE_SOURCE,
		EndpointSpecJson: spec,
	})
	if err != nil {
		return specResponse{}, err
	}

	return specResponse{
		Type:               "capture",
		DocumentationURL:   resp.DocumentationUrl,
		EndpointSpecSchema: resp.EndpointSpecSchemaJson,
		ResourceSpecSchema: resp.ResourceSpecSchemaJson,
	}, nil
}

func (cmd apiSpec) specMaterialization(ctx context.Context, spec json.RawMessage) (specResponse, error) {
	var driver, err = materialize.NewDriver(ctx,
		flow.EndpointType_FLOW_SINK, spec, cmd.Network, ops.StdLogger())
	if err != nil {
		return specResponse{}, err
	}

	resp, err := driver.Spec(ctx, &pm.SpecRequest{
		EndpointType:     flow.EndpointType_FLOW_SINK,
		EndpointSpecJson: spec,
	})
	if err != nil {
		return specResponse{}, err
	}

	return specResponse{
		Type:               "materialization",
		DocumentationURL:   resp.DocumentationUrl,
		EndpointSpecSchema: resp.EndpointSpecSchemaJson,
		ResourceSpecSchema: resp.ResourceSpecSchemaJson,
	}, nil
}

func (cmd apiSpec) Execute(_ []string) error {
	defer mbp.InitDiagnosticsAndRecover(cmd.Diagnostics)()
	mbp.InitLog(cmd.Log)

	logrus.WithFields(logrus.Fields{
		"config":    cmd,
		"version":   mbp.Version,
		"buildDate": mbp.BuildDate,
	}).Info("flowctl configuration")
	pb.RegisterGRPCDispatcher("local")

	var resp, err = cmd.execute(context.Background())
	if err != nil {
		return err
	}
	return json.NewEncoder(os.Stdout).Encode(resp)
}
